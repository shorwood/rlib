# Miette lints

## Summary

Rules for useful, safe, and consistent Miette diagnostics.

## How to enable

Enable the `miette` Cargo feature to register this family. Enable the whole family with `rlib::miette`.

## Lints

| Lint | Summary | Purpose | Fix |
| --- | --- | --- | --- |
| [`rlib::miette_ad_hoc_diagnostics_at_domain_boundaries`](./miette_ad_hoc_diagnostics_at_domain_boundaries/README.md) | Finds the same static ad-hoc Miette diagnostic or context message propagated by multiple functions when at least one occurrence crosses a public API boundary. | API design | Manual |
| [`rlib::miette_duplicate_diagnostic_codes`](./miette_duplicate_diagnostic_codes/README.md) | Finds distinct Miette diagnostic types or enum variants that expose the same static diagnostic code. | Correctness | Manual |
| [`rlib::miette_generic_diagnostic_help`](./miette_generic_diagnostic_help/README.md) | Finds static Miette help text that consists only of a configured generic phrase. | Code clarity | Manual |
| [`rlib::miette_incoherent_diagnostic_severity`](./miette_incoherent_diagnostic_severity/README.md) | Finds statically advisory Miette diagnostic types used as the error channel of `Result`-returning functions and methods, including through standard owning pointer wrappers. | Code clarity | Manual |
| [`rlib::miette_labels_without_source_code`](./miette_labels_without_source_code/README.md) | Finds Miette diagnostic labels that have no local source-code field or forwarded diagnostic source. | Correctness | Manual |
| [`rlib::miette_malformed_diagnostic_codes`](./miette_malformed_diagnostic_codes/README.md) | Finds static Miette diagnostic codes that are neither namespaced lowercase Rust paths nor conventional uppercase letter-number identifiers. | Correctness | Manual |
| [`rlib::miette_manual_diagnostic_impls`](./miette_manual_diagnostic_impls/README.md) | Finds hand-written `miette::Diagnostic` implementations containing only defaults, static metadata, or direct source-field forwarding expressible by Miette derive attributes. | API design | Manual |
| [`rlib::miette_misclassified_related_diagnostics`](./miette_misclassified_related_diagnostics/README.md) | Finds explicit causal field names marked `#[related]` and explicit sibling/aggregate names marked `#[diagnostic_source]`. | Correctness | Manual |
| [`rlib::miette_missing_diagnostic_codes`](./miette_missing_diagnostic_codes/README.md) | Finds non-transparent variants without codes in Miette diagnostic enums whose siblings establish a code policy. | Correctness | Manual |
| [`rlib::miette_plain_error_diagnostic_sources`](./miette_plain_error_diagnostic_sources/README.md) | Finds causal fields whose local type derives `miette::Diagnostic` but which forward only through `std::error::Error::source`. | Code clarity | Manual |
| [`rlib::miette_reports_in_library_interfaces`](./miette_reports_in_library_interfaces/README.md) | Finds exported functions and methods in library crate types that return `miette::Report` directly or through `miette::Result`, including behind standard owning pointers. | API design | Manual |
| [`rlib::miette_sensitive_diagnostic_source`](./miette_sensitive_diagnostic_source/README.md) | Finds strongly named credential or private-content fields exposed through `#[source_code]`. | Safety | Manual |
| [`rlib::miette_source_code_without_labels`](./miette_source_code_without_labels/README.md) | Finds Miette diagnostics that retain source code without labels or nested diagnostics that can focus the report. | Correctness | Manual |
| [`rlib::miette_unfocused_diagnostic_labels`](./miette_unfocused_diagnostic_labels/README.md) | Finds Miette diagnostics with multiple labels but no explicitly primary label. | Code clarity | Manual |
| [`rlib::miette_unstable_diagnostic_urls`](./miette_unstable_diagnostic_urls/README.md) | Finds static Miette diagnostic URLs that are relative, insecure, local-development addresses, or derived from presentation placeholders. | Correctness | Manual |
