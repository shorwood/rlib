use std::fs;
use std::path::{Path, PathBuf};

const PROJECT_LINTS: [&str; 18] = [
    "bare_tuple_types",
    "bool_fields_without_predicate_prefix",
    "collection_method_like_free_functions",
    "cross_file_struct_impls",
    "duplicate_section_divider_prefixes",
    "incoherent_type_family_names",
    "invalid_barrel_file_items",
    "malformed_section_dividers",
    "method_like_free_functions",
    "mismatched_section_divider_prefixes",
    "misordered_inherent_impl_items",
    "misordered_module_declarations",
    "misordered_type_declarations",
    "missing_section_dividers",
    "needless_function_wrappers",
    "nested_tuple_types",
    "non_adjacent_struct_impls",
    "positional_aggregate_fields",
];

#[test]
fn production_sources_do_not_allow_project_lints() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for path in rust_sources(&source_root) {
        let source = fs::read_to_string(&path).expect("production Rust source should be readable");
        for body in allow_attribute_bodies(&source) {
            for lint in PROJECT_LINTS {
                assert!(
                    !body.split(',').any(|entry| entry.trim() == lint),
                    "{} broadly allows project lint `{lint}`; fix the violation or use a narrow, reasoned expectation",
                    path.display()
                );
            }
        }
    }
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

fn allow_attribute_bodies(source: &str) -> Vec<&str> {
    let mut bodies = Vec::new();
    let mut remainder = source;
    while let Some(start) = remainder.find("allow(") {
        let body = &remainder[start + "allow(".len()..];
        let Some(end) = body.find(')') else { break };
        bodies.push(&body[..end]);
        remainder = &body[end + 1..];
    }
    bodies
}
