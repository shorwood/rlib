use std::collections::HashSet;

use super::schema::FileConfig;

// -----------------------------------------------------------------------------
// Threshold: Positive numeric policy validation
// -----------------------------------------------------------------------------

/// One named positive integer threshold.
struct Threshold<'name> {
    /// User-facing configuration key.
    name: &'name str,
    /// Configured threshold value.
    value: usize,
}

impl<'name> Threshold<'name> {
    /// Creates one threshold validation input.
    const fn new(name: &'name str, value: usize) -> Self {
        Self { name, value }
    }
}

/// Rejects zero-valued numeric thresholds.
fn positive<const N: usize>(values: [Threshold<'_>; N]) -> Result<(), String> {
    let invalid = values
        .into_iter()
        .filter_map(|threshold| (threshold.value == 0).then_some(threshold.name))
        .collect::<Vec<_>>();
    if invalid.is_empty() {
        Ok(())
    } else {
        Err(format!("{} must be greater than zero", invalid.join(", ")))
    }
}

/// Validates every numeric threshold in the flat schema.
pub(super) fn thresholds(file: &FileConfig) -> Result<(), String> {
    positive([
        Threshold::new(
            "function-phase-lines-threshold",
            file.function_phase_lines_threshold,
        ),
        Threshold::new(
            "control-flow-depth-threshold",
            file.control_flow_depth_threshold,
        ),
        Threshold::new("match-arm-lines-threshold", file.match_arm_lines_threshold),
        Threshold::new(
            "method-chain-calls-threshold",
            file.method_chain_calls_threshold,
        ),
        Threshold::new(
            "declarations-per-section-threshold",
            file.declarations_per_section_threshold,
        ),
        Threshold::new(
            "extension-trait-methods-threshold",
            file.extension_trait_methods_threshold,
        ),
        Threshold::new(
            "leptos-setup-statements-threshold",
            file.leptos_setup_statements_threshold,
        ),
        Threshold::new(
            "leptos-reactive-primitives-threshold",
            file.leptos_reactive_primitives_threshold,
        ),
        Threshold::new(
            "leptos-event-handler-statements-threshold",
            file.leptos_event_handler_statements_threshold,
        ),
        Threshold::new(
            "leptos-event-handler-control-flow-depth-threshold",
            file.leptos_event_handler_control_flow_depth_threshold,
        ),
        Threshold::new(
            "leptos-view-nesting-depth-threshold",
            file.leptos_view_nesting_depth_threshold,
        ),
        Threshold::new(
            "leptos-view-control-flow-depth-threshold",
            file.leptos_view_control_flow_depth_threshold,
        ),
        Threshold::new(
            "leptos-component-composition-depth-threshold",
            file.leptos_component_composition_depth_threshold,
        ),
        Threshold::new(
            "leptos-component-props-threshold",
            file.leptos_component_props_threshold,
        ),
        Threshold::new(
            "leptos-components-per-module-threshold",
            file.leptos_components_per_module_threshold,
        ),
        Threshold::new(
            "leptos-repeated-view-fragment-nodes-threshold",
            file.leptos_repeated_view_fragment_nodes_threshold,
        ),
        Threshold::new(
            "leptos-repeated-view-fragment-occurrences-threshold",
            file.leptos_repeated_view_fragment_occurrences_threshold,
        ),
        Threshold::new(
            "leptos-unnamed-view-complexity-threshold",
            file.leptos_unnamed_view_complexity_threshold,
        ),
        Threshold::new(
            "leptos-view-section-complexity-threshold",
            file.leptos_view_section_complexity_threshold,
        ),
        Threshold::new(
            "leptos-unnamed-view-attribute-complexity-threshold",
            file.leptos_unnamed_view_attribute_complexity_threshold,
        ),
        Threshold::new(
            "leptos-view-attribute-group-complexity-threshold",
            file.leptos_view_attribute_group_complexity_threshold,
        ),
        Threshold::new("leptos-view-max-width", file.leptos_view_max_width),
        Threshold::new("leptos-css-max-width", file.leptos_css_max_width),
    ])
}

// -----------------------------------------------------------------------------
// ConfigList: Replacement and default-splicing validation
// -----------------------------------------------------------------------------

/// Validates one user-authored identifier or phrase.
pub(super) fn name(key: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{key} entries must not be blank"))
    } else {
        Ok(())
    }
}

/// Owns replacement and default-splicing semantics for configurable lists.
pub(super) struct ConfigList;

impl ConfigList {
    /// Resolves a configured list, expanding `..` to the built-in defaults once.
    pub(super) fn resolve(
        key: &str,
        configured: Option<Vec<String>>,
        defaults: &[&str],
    ) -> Result<Vec<String>, String> {
        // An omitted list preserves all built-in defaults.
        let Some(configured) = configured else {
            return Ok(defaults.iter().map(|entry| (*entry).to_owned()).collect());
        };
        let mut resolved = Vec::new();
        let mut saw_defaults = false;
        for entry in configured {
            if entry == ".." {
                // More than one splice would duplicate the same implicit values.
                if saw_defaults {
                    return Err(format!("{key} may contain `..` at most once"));
                }
                saw_defaults = true;
                resolved.extend(defaults.iter().map(|entry| (*entry).to_owned()));
            } else {
                name(key, &entry)?;
                resolved.push(entry);
            }
        }
        let mut unique = HashSet::new();

        // Duplicate values make policy ordering ambiguous and are never meaningful.
        if let Some(duplicate) = resolved.iter().find(|entry| !unique.insert(entry.as_str())) {
            return Err(format!("{key} contains duplicate entry `{duplicate}`"));
        }
        Ok(resolved)
    }
}
