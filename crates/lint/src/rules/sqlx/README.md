# SQLx lints

## Summary

Rules for checked SQL, explicit database lifecycles, typed row decoding, and efficient query execution.

## How to enable

Enable the `sqlx` Cargo feature to register this family. Enable the whole family with `rlib::sqlx`.

## Lints

| Lint | Summary | Purpose | Fix |
| --- | --- | --- | --- |
| [`rlib::sqlx_dynamic_queries_cached_persistently`](./sqlx_dynamic_queries_cached_persistently/README.md) | Finds varying `QueryBuilder` statement shapes that retain SQLx's persistent prepared-statement caching. | Safety | Manual |
| [`rlib::sqlx_manual_row_mapping`](./sqlx_manual_row_mapping/README.md) | Finds structs populated by repeated extraction from one SQLx row. | Code clarity | Manual |
| [`rlib::sqlx_needless_pool_acquisition`](./sqlx_needless_pool_acquisition/README.md) | Finds pool connections acquired solely to execute one SQLx query. | Code clarity | Manual |
| [`rlib::sqlx_panicking_row_access`](./sqlx_panicking_row_access/README.md) | Finds SQLx row accessors that panic when a column is absent or cannot be decoded. | Safety | Manual |
| [`rlib::sqlx_queries_in_loops`](./sqlx_queries_in_loops/README.md) | Finds SQLx queries executed once per loop iteration. | Safety | Manual |
| [`rlib::sqlx_query_builder_reused_without_reset`](./sqlx_query_builder_reused_without_reset/README.md) | Finds an SQLx `QueryBuilder` reused after `build` without an intervening `reset`. | Correctness | Manual |
| [`rlib::sqlx_static_queries_without_compile_time_checks`](./sqlx_static_queries_without_compile_time_checks/README.md) | Finds static SQL passed through SQLx's runtime query functions instead of checked macros. | Correctness | Manual |
| [`rlib::sqlx_transactions_implicitly_rolled_back`](./sqlx_transactions_implicitly_rolled_back/README.md) | Finds SQLx transactions with no explicit `commit` or `rollback`. | Correctness | Manual |
| [`rlib::sqlx_unchecked_query_macros`](./sqlx_unchecked_query_macros/README.md) | Finds SQLx query macros that skip output type checking. | Correctness | Manual |
| [`rlib::sqlx_unchecked_row_decoding`](./sqlx_unchecked_row_decoding/README.md) | Finds SQLx row accessors that skip type compatibility checks. | Correctness | Manual |
| [`rlib::sqlx_unstructured_assert_sql_safe`](./sqlx_unstructured_assert_sql_safe/README.md) | Finds primitive strings passed to SQLx's `assert_sql_safe` boundary. | Safety | Manual |
| [`rlib::sqlx_unstructured_query_builder_fragments`](./sqlx_unstructured_query_builder_fragments/README.md) | Finds primitive values used as SQL syntax fragments in an SQLx `QueryBuilder`. | Safety | Manual |
