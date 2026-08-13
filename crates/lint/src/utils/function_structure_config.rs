use serde::Deserialize;

// -----------------------------------------------------------------------------
// FunctionStructureConfig: Named function readability limits
// -----------------------------------------------------------------------------

/// Limits shared by the function-structure lint family.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct FunctionStructureConfig {
    /// Maximum source lines permitted in one uncommented function phase.
    pub(super) max_phase_lines: usize,
    /// Ordinary line-comment prefix that introduces a named phase.
    pub(super) phase_comment_prefix: String,
    /// Maximum semantic control-flow nesting depth.
    pub(super) max_control_flow_depth: usize,
    /// Maximum source lines permitted in one match arm.
    pub(super) max_match_arm_lines: usize,
    /// Maximum calls permitted in one method chain.
    pub(super) max_method_chain_calls: usize,
}

impl Default for FunctionStructureConfig {
    fn default() -> Self {
        Self {
            max_phase_lines: 7,
            phase_comment_prefix: "//".to_owned(),
            max_control_flow_depth: 2,
            max_match_arm_lines: 7,
            max_method_chain_calls: 3,
        }
    }
}

impl FunctionStructureConfig {
    /// Rejects limits or comment syntax that cannot define a useful policy.
    pub(super) fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("max_phase_lines", self.max_phase_lines),
            ("max_control_flow_depth", self.max_control_flow_depth),
            ("max_match_arm_lines", self.max_match_arm_lines),
            ("max_method_chain_calls", self.max_method_chain_calls),
        ] {
            if value == 0 {
                return Err(format!(
                    "function_structure.{name} must be greater than zero"
                ));
            }
        }

        let prefix = &self.phase_comment_prefix;
        if prefix.trim() != prefix
            || prefix.contains(['\n', '\r'])
            || !prefix.starts_with("//")
            || prefix.starts_with("///")
            || prefix.starts_with("//!")
        {
            return Err(
                "function_structure.phase_comment_prefix must be one trimmed ordinary `//` comment prefix"
                    .to_owned(),
            );
        }
        Ok(())
    }
}
