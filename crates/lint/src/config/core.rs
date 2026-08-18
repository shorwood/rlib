// -----------------------------------------------------------------------------
// FunctionStructureConfig: Function complexity policy
// -----------------------------------------------------------------------------

/// Limits and fixed syntax shared by function-structure lints.
#[derive(Clone)]
#[expect(
    clippy::struct_field_names,
    reason = "the shared maximum prefix distinguishes enforced limits from measured values"
)]
pub struct FunctionStructureConfig {
    /// Maximum lines in one unnamed code phase.
    pub(crate) max_phase_lines: usize,
    /// Maximum nested control-flow depth.
    pub(crate) max_control_flow_depth: usize,
    /// Maximum lines in one match arm.
    pub(crate) max_match_arm_lines: usize,
    /// Maximum calls in one method chain.
    pub(crate) max_method_chain_calls: usize,
}

impl FunctionStructureConfig {
    /// Fixed prefix recognized for code-phase comments.
    pub(crate) const PHASE_COMMENT_PREFIX: &str = "//";
}

// -----------------------------------------------------------------------------
// SectionDividerConfig: Module section policy
// -----------------------------------------------------------------------------

/// Canonical source template for module section dividers.
const SECTION_DIVIDER_TEMPLATE: &str = "// -----------------------------------------------------------------------------\n\
     // {content}\n\
     // -----------------------------------------------------------------------------";

/// Limits and fixed syntax shared by module-section lints.
#[derive(Clone)]
pub struct SectionDividerConfig {
    /// Maximum declarations allowed in one authored section.
    pub(crate) max_declarations_per_section: usize,
}

impl SectionDividerConfig {
    /// Canonical three-line section divider template.
    pub(crate) const TEMPLATE: &str = SECTION_DIVIDER_TEMPLATE;

    /// Maximum accepted divider line width.
    pub(crate) const MAX_LINE_LENGTH: usize = 80;
}

// -----------------------------------------------------------------------------
// ExtensionTraitConfig: Extension trait policy
// -----------------------------------------------------------------------------

/// Limits shared by extension-trait lints.
#[derive(Clone)]
pub struct ExtensionTraitConfig {
    /// Maximum methods in one coherent extension trait.
    pub(crate) max_methods: usize,
}
