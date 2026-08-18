# thiserror lints

## Summary

Rules for clear thiserror messages, complete source chains, safe public errors, and replaceable hand-written code.

## How to enable

Enable the `thiserror` Cargo feature to register this family. Enable the whole family with `rlib::thiserror`.

## Lints

| Lint | Summary | Purpose | Fix |
| --- | --- | --- | --- |
| [`rlib::thiserror_ambiguous_error_sources`](./thiserror_ambiguous_error_sources/README.md) | Finds derived thiserror types that store several likely causes, even though at most one is exposed through `Error::source`. | Correctness | Manual |
| [`rlib::thiserror_duplicate_error_messages`](./thiserror_duplicate_error_messages/README.md) | Finds distinct variants in a `thiserror::Error` enum with identical static `#[error("...")]` messages. | Correctness | Manual |
| [`rlib::thiserror_dynamic_errors_in_library_interfaces`](./thiserror_dynamic_errors_in_library_interfaces/README.md) | Finds public functions whose `Result` error type is `Box<dyn Error>`, `anyhow::Error`, or `miette::Report`. | Code clarity | Manual |
| [`rlib::thiserror_error_messages_used_as_identifiers`](./thiserror_error_messages_used_as_identifiers/README.md) | Finds equality, prefix, suffix, and substring decisions made from standard `ToString` output of a value whose type derives `thiserror::Error`. | Code clarity | Manual |
| [`rlib::thiserror_from_sources_without_context`](./thiserror_from_sources_without_context/README.md) | Finds transparent `#[from]` variants that receive failures from two or more differently named operations in the same function. | API design | Manual |
| [`rlib::thiserror_manual_error_impls`](./thiserror_manual_error_impls/README.md) | Finds hand-written `Display` and `std::error::Error` implementations whose complete behavior is reproducible by `thiserror::Error`. | API design | Manual |
| [`rlib::thiserror_manual_from_error_variants`](./thiserror_manual_from_error_variants/README.md) | Finds manual `From<SourceError>` implementations that only construct a one-field source-bearing variant of a type already deriving `thiserror::Error`. | Code clarity | Manual |
| [`rlib::thiserror_non_send_sync_public_errors`](./thiserror_non_send_sync_public_errors/README.md) | Finds public thiserror types sent through public channel APIs even though one of their fields is known to be limited to the current thread. | Safety | Manual |
| [`rlib::thiserror_opaque_errors_exposing_representations`](./thiserror_opaque_errors_exposing_representations/README.md) | Finds public derived thiserror enums whose source-bearing variants directly expose concrete error types from another crate. | API design | Manual |
| [`rlib::thiserror_unpropagated_error_backtraces`](./thiserror_unpropagated_error_backtraces/README.md) | Finds local thiserror wrappers that recapture a backtrace already provided by their source or fail to forward that source backtrace. | Safety | Manual |
| [`rlib::thiserror_unreported_error_sources`](./thiserror_unreported_error_sources/README.md) | Finds a single likely cause that is itself a local thiserror type but is omitted from the containing error's `Error::source` chain. | Correctness | Manual |
