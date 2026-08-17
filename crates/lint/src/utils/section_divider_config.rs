use serde::Deserialize;

// -----------------------------------------------------------------------------
// SectionDividerConfig: Module section divider rendering
// -----------------------------------------------------------------------------

/// Shared configuration for the section-divider lint family.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct SectionDividerConfig {
    /// Multiline divider template containing the required `{content}` placeholder.
    pub(super) template: String,
    /// Maximum rendered divider line width.
    pub(super) max_line_length: usize,
    /// Maximum declarations governed by one section divider.
    pub(super) max_declarations_per_section: usize,
}

impl Default for SectionDividerConfig {
    fn default() -> Self {
        Self {
            template: "// -----------------------------------------------------------------------------\n\
                       // {content}\n\
                       // -----------------------------------------------------------------------------"
                .to_owned(),
            max_line_length: 80,
            max_declarations_per_section: 5,
        }
    }
}

impl SectionDividerConfig {
    /// Rejects section policies that cannot contain a declaration.
    pub(super) fn validate(&self) -> Result<(), String> {
        // A section limit of zero cannot contain any declaration.
        if self.max_declarations_per_section == 0 {
            return Err(
                "section_dividers.max_declarations_per_section must be greater than zero"
                    .to_owned(),
            );
        }
        Ok(())
    }
}
