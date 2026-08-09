use serde::Deserialize;

// -----------------------------------------------------------------------------
// ExtensionTraitConfig: Focused extension trait limits
// -----------------------------------------------------------------------------

/// Limits that prevent extension traits from becoming catch-all APIs.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExtensionTraitConfig {
    /// Maximum number of methods permitted on one extension trait.
    pub(crate) max_methods: usize,
}

impl Default for ExtensionTraitConfig {
    fn default() -> Self {
        Self { max_methods: 8 }
    }
}

impl ExtensionTraitConfig {
    /// Rejects a method budget that cannot admit any extension behavior.
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.max_methods == 0 {
            return Err("extension_traits.max_methods must be greater than zero".to_owned());
        }
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// LibraryConfig: Complete lint library configuration
// -----------------------------------------------------------------------------

/// Every configurable policy exposed through the `rlib-lint` Dylint table.
#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LibraryConfig {
    /// Size policy used by focused extension-trait lints.
    pub(crate) extension_traits: ExtensionTraitConfig,
    /// Limits and syntax used by the function-structure lint family.
    pub(crate) function_structure: FunctionStructureConfig,
    /// Rendering and width policy used by section-divider lints.
    pub(crate) section_dividers: SectionDividerConfig,
}

impl LibraryConfig {
    /// Loads the complete library configuration from Dylint's process environment.
    pub(crate) fn load() -> Self {
        dylint_linting::config_or_default(env!("CARGO_PKG_NAME"))
    }
}

// -----------------------------------------------------------------------------
// FunctionStructureConfig: Named function readability limits
// -----------------------------------------------------------------------------

/// Limits shared by the function-structure lint family.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FunctionStructureConfig {
    /// Maximum number of source lines permitted in one unnamed function phase.
    pub(crate) max_phase_lines: usize,
    /// Ordinary line-comment prefix that introduces a named phase.
    pub(crate) phase_comment_prefix: String,
    /// Maximum permitted nesting depth for control-flow expressions.
    pub(crate) max_control_flow_depth: usize,
    /// Maximum source-line span permitted for one match arm body.
    pub(crate) max_match_arm_lines: usize,
    /// Maximum number of calls permitted in one method-call chain.
    pub(crate) max_method_chain_calls: usize,
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
    /// Rejects values that would make source analysis ambiguous or degenerate.
    pub(crate) fn validate(&self) -> Result<(), String> {
        // Numeric limits must leave every policy with a meaningful nonzero boundary.
        for (name, value) in [
            ("max_phase_lines", self.max_phase_lines),
            ("max_control_flow_depth", self.max_control_flow_depth),
            ("max_match_arm_lines", self.max_match_arm_lines),
            ("max_method_chain_calls", self.max_method_chain_calls),
        ] {
            if value != 0 {
                continue;
            }
            return Err(format!(
                "function_structure.{name} must be greater than zero"
            ));
        }

        // The phase marker must remain an ordinary, single-line Rust comment prefix.
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

// -----------------------------------------------------------------------------
// SectionDividerConfig: Module section divider rendering
// -----------------------------------------------------------------------------

/// Shared configuration for the section-divider lint family.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SectionDividerConfig {
    /// Divider template containing the required `{content}` placeholder.
    pub(crate) template: String,
    /// Maximum rendered divider width, including the section content.
    pub(crate) max_line_length: usize,
    /// Maximum number of distinct declarations governed by one divider.
    pub(crate) max_declarations_per_section: usize,
}

impl Default for SectionDividerConfig {
    fn default() -> Self {
        // Keep the default divider readable under the ordinary source width.
        let template = "// -----------------------------------------------------------------------------\n\
                        // {content}\n\
                        // -----------------------------------------------------------------------------"
            .to_owned();

        // Bound each conceptual family independently from its rendered syntax.
        Self {
            max_line_length: 80,
            max_declarations_per_section: 5,
            template,
        }
    }
}

impl SectionDividerConfig {
    /// Rejects limits that cannot describe a useful declaration section.
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.max_declarations_per_section == 0 {
            return Err(
                "section_dividers.max_declarations_per_section must be greater than zero"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExtensionTraitConfig, FunctionStructureConfig, LibraryConfig, SectionDividerConfig,
    };

    #[test]
    fn parses_custom_extension_trait_limit() {
        let config = toml::from_str::<LibraryConfig>(
            r"
                [extension_traits]
                max_methods = 5
            ",
        )
        .expect("custom extension trait limit should parse");
        assert_eq!(config.extension_traits.max_methods, 5);
        assert!(config.extension_traits.validate().is_ok());
    }

    #[test]
    fn uses_eight_extension_methods_by_default() {
        assert_eq!(ExtensionTraitConfig::default().max_methods, 8);
    }

    #[test]
    fn rejects_zero_extension_method_limit() {
        let config = ExtensionTraitConfig { max_methods: 0 };
        assert!(config.validate().is_err());
    }

    #[test]
    fn parses_custom_function_structure_limits() {
        let config = toml::from_str::<LibraryConfig>(
            r#"
                [function_structure]
                max_phase_lines = 11
                phase_comment_prefix = "// Phase:"
                max_control_flow_depth = 3
                max_match_arm_lines = 13
                max_method_chain_calls = 5
            "#,
        )
        .expect("custom function structure should parse");
        assert_eq!(config.function_structure.max_phase_lines, 11);
        assert_eq!(config.function_structure.phase_comment_prefix, "// Phase:");
        assert_eq!(config.function_structure.max_control_flow_depth, 3);
        assert_eq!(config.function_structure.max_match_arm_lines, 13);
        assert_eq!(config.function_structure.max_method_chain_calls, 5);
        assert!(config.function_structure.validate().is_ok());
    }

    #[test]
    fn uses_natural_line_comments_by_default() {
        assert_eq!(
            FunctionStructureConfig::default().phase_comment_prefix,
            "//"
        );
    }

    #[test]
    fn rejects_zero_limits_and_non_comment_prefixes() {
        let config = FunctionStructureConfig {
            max_phase_lines: 0,
            ..FunctionStructureConfig::default()
        };
        assert!(config.validate().is_err());

        let config = FunctionStructureConfig {
            phase_comment_prefix: "---".to_owned(),
            ..FunctionStructureConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn parses_custom_section_declaration_limit() {
        let config = toml::from_str::<LibraryConfig>(
            r"
                [section_dividers]
                max_declarations_per_section = 12
            ",
        )
        .expect("custom section declaration limit should parse");

        assert_eq!(config.section_dividers.max_declarations_per_section, 12);
        assert!(config.section_dividers.validate().is_ok());
    }

    #[test]
    fn uses_five_declarations_per_section_by_default() {
        assert_eq!(
            SectionDividerConfig::default().max_declarations_per_section,
            5
        );
    }

    #[test]
    fn rejects_zero_section_declaration_limit() {
        let config = SectionDividerConfig {
            max_declarations_per_section: 0,
            ..SectionDividerConfig::default()
        };
        assert!(config.validate().is_err());
    }
}
