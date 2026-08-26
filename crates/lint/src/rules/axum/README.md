# Axum lints

## Summary

Rules for giving response rendering a typed `IntoResponse` owner while keeping handlers focused on
request orchestration.

## How to enable

Enable the `axum` Cargo feature to register this family. Enable the whole family with `rlib::axum`.

## Lints

| Lint | Summary | Purpose | Fix |
| --- | --- | --- | --- |
| [`rlib::axum_ad_hoc_response_wrappers`](./axum_ad_hoc_response_wrappers/README.md) | Finds synchronous Axum response-producing functions and methods whose rendering behavior should be owned by an `IntoResponse` implementation. | API design | Manual |
