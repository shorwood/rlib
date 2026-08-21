# Leptos lints

## Summary

Rules for Leptos components, views, reactivity, event handlers, resources, and server functions.

## How to enable

Enable the `leptos` Cargo feature to register this family. Enable the whole family with `rlib::leptos`.

## Lints

| Lint | Summary | Purpose | Fix |
| --- | --- | --- | --- |
| [`rlib::leptos_attribute_bound_controlled_inputs`](./leptos_attribute_bound_controlled_inputs/README.md) | Checks for writable reactive values passed to the plain `value` or `checked` HTML attribute of a form control. | Code clarity | Manual |
| [`rlib::leptos_boolean_component_props`](./leptos_boolean_component_props/README.md) | Warns about direct, optional, and reactive boolean properties on Leptos components when their names do not communicate a standard binary state. | Code clarity | Manual |
| [`rlib::leptos_duplicate_view_section_comments`](./leptos_duplicate_view_section_comments/README.md) | Checks that section headings in the same part of a `view!` have distinct names. | Correctness | Manual |
| [`rlib::leptos_effects_synchronizing_signals`](./leptos_effects_synchronizing_signals/README.md) | Checks for Leptos effects that read tracked reactive state and write reactive state in the same effect callback. | Code clarity | Manual |
| [`rlib::leptos_excessive_component_composition_depth`](./leptos_excessive_component_composition_depth/README.md) | Limits the longest acyclic chain in the crate-local component call graph and reports its root once. | Code clarity | Manual |
| [`rlib::leptos_excessive_component_props`](./leptos_excessive_component_props/README.md) | Limits component props while excluding Leptos `Children*` composition props. | Code clarity | Manual |
| [`rlib::leptos_excessively_nested_views`](./leptos_excessively_nested_views/README.md) | Limits static tag nesting and embedded Rust control-flow nesting within a hand-written component view. | Code clarity | Manual |
| [`rlib::leptos_fragmented_reactive_state`](./leptos_fragmented_reactive_state/README.md) | Limits the reactive primitives created directly by each component and composable. | Code clarity | Manual |
| [`rlib::leptos_hydration_divergent_views`](./leptos_hydration_divergent_views/README.md) | Finds compile-time server/browser branches that author different initial `view!` node shapes, including element nesting and text-node presence. | Safety | Manual |
| [`rlib::leptos_implicit_default_component_props`](./leptos_implicit_default_component_props/README.md) | Checks for Leptos component properties that use `#[prop(optional)]` with a concrete value type. | Code clarity | Manual |
| [`rlib::leptos_malformed_view_section_comments`](./leptos_malformed_view_section_comments/README.md) | Checks the spelling and placement of section comments inside `view!`. | Code clarity | Automatic |
| [`rlib::leptos_manual_resource_refetch_signals`](./leptos_manual_resource_refetch_signals/README.md) | Checks for a signal value that is read and immediately discarded inside a `LocalResource` or `ArcLocalResource` fetcher. | Code clarity | Manual |
| [`rlib::leptos_markup_repeating_view_comments`](./leptos_markup_repeating_view_comments/README.md) | Checks for one-node view sections whose heading only repeats the node’s name or visible label. | Code clarity | Manual |
| [`rlib::leptos_mismatched_view_attribute_groups`](./leptos_mismatched_view_attribute_groups/README.md) | Finds Leptos attributes placed beneath a group heading that explicitly names a contradictory behavioral category. | Correctness | Manual |
| [`rlib::leptos_missing_view_attribute_group_comments`](./leptos_missing_view_attribute_group_comments/README.md) | Finds dense Leptos opening tags whose attributes span several responsibilities without named groups. | Code clarity | Manual |
| [`rlib::leptos_missing_view_section_comments`](./leptos_missing_view_section_comments/README.md) | Checks complex groups of direct children in `view!` for explanatory section headings. | Code clarity | Manual |
| [`rlib::leptos_needlessly_cloned_signal_values`](./leptos_needlessly_cloned_signal_values/README.md) | Checks for a non-copy signal value read with `get()` and immediately inspected with `len()` or `is_empty()`. | Code clarity | Manual |
| [`rlib::leptos_noncanonical_view_formatting`](./leptos_noncanonical_view_formatting/README.md) | Formats hand-written Leptos `view!` macros with a deterministic leptosfmt policy and reports files whose markup differs from that standard rendering. | Style | Automatic |
| [`rlib::leptos_overpopulated_component_modules`](./leptos_overpopulated_component_modules/README.md) | Limits hand-written component and island definitions per source or inline module. | Code clarity | Manual |
| [`rlib::leptos_oversized_event_handlers`](./leptos_oversized_event_handlers/README.md) | Limits statement count and nested control flow in `on:*` handlers and `Callback::new` closures. | Code clarity | Manual |
| [`rlib::leptos_oversized_reactive_setups`](./leptos_oversized_reactive_setups/README.md) | Limits top-level setup statements in components and `use_*` composables, excluding the returned tail expression. | Code clarity | Manual |
| [`rlib::leptos_oversized_view_attribute_groups`](./leptos_oversized_view_attribute_groups/README.md) | Finds named Leptos attribute groups whose direct complexity exceeds the configured limit. | Code clarity | Manual |
| [`rlib::leptos_oversized_view_sections`](./leptos_oversized_view_sections/README.md) | Checks that one named section inside `view!` does not contain too much direct structure. | Code clarity | Manual |
| [`rlib::leptos_primitive_context_values`](./leptos_primitive_context_values/README.md) | Finds primitive, generic-container, callback, and unbranded reactive values used as Leptos context identities. | Code clarity | Manual |
| [`rlib::leptos_reactive_writes_during_view_construction`](./leptos_reactive_writes_during_view_construction/README.md) | Checks for reactive state written directly while a suspended Leptos view is being resolved. | Safety | Manual |
| [`rlib::leptos_reactive_writes_in_resource_fetchers`](./leptos_reactive_writes_in_resource_fetchers/README.md) | Checks for signal writes performed inside the asynchronous function that loads a Leptos resource. | Safety | Manual |
| [`rlib::leptos_read_then_replace_signals`](./leptos_read_then_replace_signals/README.md) | Checks for a writable signal that reads its current value to calculate the value passed back to `set()` or `try_set()`. | Code clarity | Manual |
| [`rlib::leptos_repeated_view_fragments`](./leptos_repeated_view_fragments/README.md) | Finds crate-wide normalized RSX subtrees repeated often enough to represent a missing component. | Code clarity | Manual |
| [`rlib::leptos_resource_fetchers_rereading_sources`](./leptos_resource_fetchers_rereading_sources/README.md) | Finds `Resource` fetchers that reread a reactive value already tracked by their source closure, whether or not they also use the supplied source argument. | Code clarity | Manual |
| [`rlib::leptos_server_functions_without_authorization_boundaries`](./leptos_server_functions_without_authorization_boundaries/README.md) | Finds Leptos server functions whose configured sensitive calls are not preceded by configured authorization evidence or covered by an explicit endpoint marker. | Code clarity | Manual |
| [`rlib::leptos_static_str_component_props`](./leptos_static_str_component_props/README.md) | Checks for `&'static str` carried by Leptos component properties, including values nested in generic wrappers and locally defined carrier types. | Style | Manual |
| [`rlib::leptos_manual_view_iteration`](./leptos_manual_view_iteration/README.md) | Checks for standard iterator mapping adapters in chains completed by Leptos `collect_view()`. | Code clarity | Manual |
| [`rlib::leptos_unnamed_composables`](./leptos_unnamed_composables/README.md) | Requires project-local helpers that own reactive behavior and are consumed by components or composables to start with `use_`. | Style | Manual |
| [`rlib::leptos_unreactive_signal_reads_in_views`](./leptos_unreactive_signal_reads_in_views/README.md) | Checks for tracked signal reads evaluated directly while a Leptos view is first constructed, including fallible clone, guard, and closure-based reads. | Code clarity | Manual |
| [`rlib::leptos_unsanitized_inner_html`](./leptos_unsanitized_inner_html/README.md) | Checks for runtime `String` and `&str` values passed directly to Leptos's `inner_html` attribute. | Safety | Manual |
| [`rlib::leptos_unscoped_spawned_tasks`](./leptos_unscoped_spawned_tasks/README.md) | Warns about direct or aliased `spawn_local` calls inside components and composables. | Safety | Manual |
| [`rlib::leptos_unstable_for_keys`](./leptos_unstable_for_keys/README.md) | Checks parsed `<For>` keys that do not derive from the row or that return the position of an enumerated collection. | Correctness | Manual |
| [`rlib::leptos_writable_signal_component_props`](./leptos_writable_signal_component_props/README.md) | Warns about direct and optional component properties proven to implement Leptos's reactive `Write`, `Set`, `Update`, or `UpdateUntracked` capabilities. | Code clarity | Manual |
