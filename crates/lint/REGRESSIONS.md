# Semantic regression ledger

This ledger tracks recurring false-positive and false-negative patterns discovered through UI
fixtures and downstream dogfooding. It complements snapshots: snapshots lock rendered output,
while this file records why a semantic boundary exists and where its regression evidence lives.

## Recording policy

- Record a pattern when it affects more than one lint, appears in a real consumer, or can silently
  weaken a semantic rule.
- Classify false positives as **scope leaks** (non-authored code entered analysis), **identity
  mistakes** (source spelling overrode resolved meaning), or **evidence mistakes** (real intent was
  ignored).
- Classify false negatives as **scope gaps**, **identity gaps**, or **evidence gaps** using the same
  distinction.
- Every fixed entry must name the shared analysis boundary and at least one executable UI fixture.
- Keep positive and negative examples together when they define opposite sides of one boundary.
  Comments beginning with `False-positive boundary` or `False-negative boundary` make those cases
  searchable without introducing a second test harness.

## Active patterns

| Pattern | Failure mode | Shared boundary | Regression evidence |
| --- | --- | --- | --- |
| Generated framework component glue resembles authored functions and fields | False positive: scope leak | `source_provenance` and `free_function_analysis` | `ui/core/collection_method_like_free_functions/semantic.rs`, `ui/core/method_like_free_functions/semantic.rs`, `ui/core/bool_fields_without_predicate_prefix/semantic.rs` |
| Compiler-mandated proc-macro entry points resemble public helper APIs | False positive: scope leak | crate type and external-symbol classification | `ui/core/invalid_barrel_file_items/proc_macro.rs` |
| Type aliases hide the nominal receiver or collection element | False negative: identity gap | compiler type resolution before source-syntax migration checks | `ui/core/method_like_free_functions/semantic.rs`, `ui/core/collection_method_like_free_functions/semantic.rs`, `ui/core/foreign_type_method_like_free_functions/semantic.rs` |
| Repeated infrastructure competes with a foreign semantic subject | False positive: evidence mistake | crate-wide ambient-dependency inference | `ui/core/foreign_type_method_like_free_functions/semantic.rs` |
| Local macro output is editable but external macro output is not | False positive or false negative: scope mismatch | expansion provenance in `free_function_analysis` | all three `ui/core/*_method_like_free_functions/semantic.rs` fixtures |
| Active test implementations or consumers prove substitution | False positive: evidence mistake | compilation-local test evidence | `ui/core/single_implementation_traits/active_tests.rs`, `ui/core/unconsumed_generic_abstractions/active_tests.rs` |
| Generated Leptos props must map back to authored component fields without treating arbitrary wrappers as boolean state | False positive or false negative: identity gap | semantic `Leptos::Props` resolution and accepted-wrapper recursion | `ui/leptos/boolean_component_props/semantic.rs` |
| Standard HTML and ARIA states already give `disabled` and `invalid` booleans a stable binary vocabulary | False positive: evidence mistake | exact platform-state vocabulary in `boolean_component_props` | `boolean_component_props` unit test, downstream `leptos-pencil` dogfood |
| Infrastructure parameters must not displace a local input or local success return as the stronger API owner | False positive: evidence mistake | local ownership precedence in `foreign_type_analysis` | `ui/core/foreign_type_method_like_free_functions/semantic.rs`, downstream `flowgraph` dogfood |
| A terminal conditional whose body already diverges is an exit handler, not wrapped main-path work | False positive: evidence mistake | divergence-aware trailing-condition analysis | `ui/core/needlessly_nested_control_flow/semantic.rs`, downstream `flowgraph` dogfood |

## Triage workflow

1. Reduce the report to the smallest compiling example and decide whether the error concerns
   authored scope, resolved identity, or missing semantic evidence.
2. Search this ledger and the named boundary comments before adding a lint-local exception.
3. Put shared eligibility and identity rules in `utils`; keep proposition-specific thresholds in
   the lint analyzer.
4. Add the suppressing example and the nearest warning-producing counterpart to a UI fixture.
5. Verify the focused snapshot, the complete UI suite, and at least one downstream workspace when
   the pattern originated during dogfooding.
